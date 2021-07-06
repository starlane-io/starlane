/*
Copyright 2021.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package controllers

import (
	"context"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/errors"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"

	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"

	starlanev1alpha1 "github.com/mechtronium/starlane/api/v1alpha1"
	batchv1 "k8s.io/api/batch/v1"
)

// StarlaneProvisioningJobReconciler reconciles a StarlaneProvisioningJob object
type StarlaneProvisioningJobReconciler struct {
	client.Client
	Scheme *runtime.Scheme
}

//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlaneprovisioningjobs,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlaneprovisioningjobs/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlaneprovisioningjobs/finalizers,verbs=update
//+kubebuilder:rbac:groups=apps,resources=jobs,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=pods,verbs=get;list;

// Reconcile is part of the main kubernetes reconciliation loop which aims to
// move the current state of the cluster closer to the desired state.
// TODO(user): Modify the Reconcile function to compare the state specified by
// the StarlaneProvisioningJob object against the actual cluster state, and then
// perform operations to make the cluster state reflect the state specified by
// the user.
//
// For more details, check Reconcile and its Result here:
// - https://pkg.go.dev/sigs.k8s.io/controller-runtime@v0.8.3/pkg/reconcile
func (r *StarlaneProvisioningJobReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	log := log.FromContext(ctx)
	// Check if the job already exists, if not create a new one

	provisioning_job := &starlanev1alpha1.StarlaneProvisioningJob{}
	err := r.Get(ctx, req.NamespacedName, provisioning_job)
	if err != nil {
		if errors.IsNotFound(err) {
			// Request object not found, could have been deleted after reconcile request.
			// Owned objects are automatically garbage collected. For additional cleanup logic use finalizers.
			// Return and don't requeue
			log.Info("Starlane resource not found. Ignoring since object must be deleted")
			return ctrl.Result{}, nil
		}
		// Error reading the object - requeue the request.
		log.Error(err, "Failed to get StarlaneProvisioningJob")
		return ctrl.Result{}, err
	}

	provisioner := &starlanev1alpha1.StarlaneProvisioner{}
	provisioner_name := types.NamespacedName{
		Namespace: provisioning_job.Namespace,
		Name:      provisioning_job.Spec.Provisioner,
	}

	err = r.Get(ctx, provisioner_name, provisioner)

	if err != nil {
		if errors.IsNotFound(err) {
			log.Error(err, "could not find the provisioner: %s", provisioner_name)
			return ctrl.Result{}, nil
		}
		// Error reading the object - requeue the request.
		log.Error(err, "Failed to get Starlane Provisioner")
		return ctrl.Result{}, err
	}

	job := &batchv1.Job{}
	err = r.Get(ctx, req.NamespacedName, job)

	if err != nil && errors.IsNotFound(err) {
		// Define a new job
		dep := r.provisioningJob(provisioning_job, provisioner)
		log.Info("Creating a new Job", "Job.Namespace", dep.Namespace, "Job.Name", dep.Name)
		err = r.Create(ctx, dep)
		if err != nil {
			log.Error(err, "Failed to create new Job", "Job.Namespace", dep.Namespace, "Job.Name", dep.Name)
			return ctrl.Result{}, err
		}
		// Deployment created successfully - return and requeue
		return ctrl.Result{Requeue: true}, nil
	} else if err != nil {
		log.Error(err, "Failed to get Job")
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneProvisioningJobReconciler) provisioningJob(m *starlanev1alpha1.StarlaneProvisioningJob, p *starlanev1alpha1.StarlaneProvisioner) *batchv1.Job {

	commandArgs := []string{"create", m.Spec.StarlaneResourceAddress, m.Spec.ResourceName}
	initArgs := append(commandArgs, m.Spec.InitArgs...)

	dep := &batchv1.Job{
		ObjectMeta: metav1.ObjectMeta{
			Name:      m.Name,
			Namespace: m.Namespace,
		},
		Spec: batchv1.JobSpec{
			Template: corev1.PodTemplateSpec{
				Spec: corev1.PodSpec{
					RestartPolicy: corev1.RestartPolicyNever,
					Containers: []corev1.Container{{
						Image: p.Spec.Image,
						Name:  "starlane",
						Args:  initArgs,
						Env:   p.Spec.Env,
					}},
				},
			},
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// SetupWithManager sets up the controller with the Manager.
func (r *StarlaneProvisioningJobReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&starlanev1alpha1.StarlaneProvisioningJob{}).
		Owns(&batchv1.Job{}).
		Complete(r)
}
