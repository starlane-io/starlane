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
	"github.com/sethvargo/go-password/password"
	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	"k8s.io/apimachinery/pkg/util/intstr"
	"time"

	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"

	starlanev1alpha1 "github.com/mechtronium/starlane/api/v1alpha1"
)

// PostgresReconciler reconciles a Postgres object
type PostgresReconciler struct {
	client.Client
	Scheme *runtime.Scheme
}

//+kubebuilder:rbac:groups=starlane.starlane.io,resources=postgres,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=postgres/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=postgres/finalizers,verbs=update
//+kubebuilder:rbac:groups=apps,resources=deployments,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=pods,verbs=get;list;
//+kubebuilder:rbac:groups=core,resources=services,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=secrets,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=persistentvolumeclaims,verbs=get;list;watch;create;update;patch;delete

// Reconcile is part of the main kubernetes reconciliation loop which aims to
// move the current state of the cluster closer to the desired state.
// TODO(user): Modify the Reconcile function to compare the state specified by
// the Postgres object against the actual cluster state, and then
// perform operations to make the cluster state reflect the state specified by
// the user.
//
// For more details, check Reconcile and its Result here:
// - https://pkg.go.dev/sigs.k8s.io/controller-runtime@v0.11.2/pkg/reconcile
func (r *PostgresReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	log := log.FromContext(ctx)
	// Fetch the Postgres instance
	postgres := &starlanev1alpha1.Postgres{}

	err := r.Get(ctx, req.NamespacedName, postgres)

	pvc := &corev1.PersistentVolumeClaim{}
	err = r.Get(ctx, types.NamespacedName{Name: postgres.Name, Namespace: postgres.Namespace}, pvc)
	if err != nil && errors.IsNotFound(err) {
		// Define a new deployment
		dep := r.postgresPvc(postgres)
		log.Info("Creating a new Pvc", "Pvc.Namespace", dep.Namespace, "Pvc.Name", dep.Name)
		err = r.Create(ctx, dep)
		if err != nil {
			log.Error(err, "Failed to create new Postgres PVC", "Pvc.Namespace", dep.Namespace, "Pvc.Name", dep.Name)
			return ctrl.Result{}, err
		}
		// Pvc created successfully - return and requeue
		return ctrl.Result{Requeue: true}, nil
	} else if err != nil {
		log.Error(err, "Failed to get Pvc")
		return ctrl.Result{}, err
	}

	// secret
	{
		secret := &corev1.Secret{}
		err = r.Get(ctx, types.NamespacedName{Name: postgres.Name, Namespace: postgres.Namespace}, secret)
		if err != nil && errors.IsNotFound(err) {
			// Define a new deployment
			dep, gen_err := r.generateSecret(postgres)
			if gen_err != nil {
				log.Error(gen_err, "Failed to create a password", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, gen_err
			}
			log.Info("Creating a new Secret", "Namespace", dep.Namespace, "Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Postgres Secret", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Secret")
			return ctrl.Result{}, err
		}
	}

	deployment := &appsv1.Deployment{}
	err = r.Get(ctx, types.NamespacedName{Name: postgres.Name, Namespace: postgres.Namespace}, deployment)
	if err != nil && errors.IsNotFound(err) {
		dep := r.postgresDeployment(postgres)
		log.Info("Creating a new Postgres deployment", "Deployment.Namespace", dep.Namespace, "Deployment.Name", dep.Name)
		err = r.Create(ctx, dep)
		if err != nil {
			log.Error(err, "Failed to create new Postgres", "Deployment.Namespace", dep.Namespace, "Deployment.Name", dep.Name)
			return ctrl.Result{}, err
		}
		// Pvc created successfully - return and requeue
		return ctrl.Result{Requeue: true}, nil
	} else if err != nil {
		log.Error(err, "Failed to get Secret")
		return ctrl.Result{}, err
	}

	service := &corev1.Service{}
	err = r.Get(ctx, types.NamespacedName{Name: postgres.Name, Namespace: postgres.Namespace}, service)
	if err != nil && errors.IsNotFound(err) {
		dep := r.postgresService(postgres)
		log.Info("Creating a new Postgres service", "Namespace", dep.Namespace, "Name", dep.Name)
		err = r.Create(ctx, dep)
		if err != nil {
			log.Error(err, "Failed to create new Postgres Service", "Namespace", dep.Namespace, "Name", dep.Name)
			return ctrl.Result{}, err
		}
		// Pvc created successfully - return and requeue
		return ctrl.Result{Requeue: true}, nil
	} else if err != nil {
		log.Error(err, "Failed to get Service")
		return ctrl.Result{}, err
	} else if service.Spec.Type != postgres.Spec.ServiceType {
		if postgres.Spec.ServiceType == "" {
			service.Spec.Type = "ClusterIp"
		} else {
			service.Spec.Type = postgres.Spec.ServiceType
		}

		err = r.Update(ctx, service)
		if err != nil {
			log.Error(err, "Failed to update Service", "Service.Namespace", service.Namespace, "Service.Name", service.Name)
			return ctrl.Result{}, err
		}
		// Ask to requeue after 1 minute in order to give enough time for the
		// pods be created on the cluster side and the operand be able
		// to do the next update step accurately.
		return ctrl.Result{RequeueAfter: time.Minute}, nil
	}
	return ctrl.Result{}, nil
}

// SetupWithManager sets up the controller with the Manager.
func (r *PostgresReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&starlanev1alpha1.Postgres{}).
		Complete(r)
}

func (r *PostgresReconciler) postgresPvc(m *starlanev1alpha1.Postgres) *corev1.PersistentVolumeClaim {
	name := m.Name
	dep := &corev1.PersistentVolumeClaim{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: corev1.PersistentVolumeClaimSpec{StorageClassName: &m.Spec.StorageClass,
			AccessModes: []corev1.PersistentVolumeAccessMode{"ReadWriteOnce"},
			Resources: corev1.ResourceRequirements{
				Requests: corev1.ResourceList{
					corev1.ResourceStorage: resource.MustParse("10Gi"),
				},
			}},
	}
	if m.Spec.ManagePvc {
		ctrl.SetControllerReference(m, dep, r.Scheme)
	}
	return dep
}

// deploymentForStarlane returns a memcached Deployment object
func (r *PostgresReconciler) postgresDeployment(m *starlanev1alpha1.Postgres) *appsv1.Deployment {

	ls := map[string]string{"name": m.Name}
	replicas := int32(1)

	dep := &appsv1.Deployment{
		ObjectMeta: metav1.ObjectMeta{
			Name:      m.Name,
			Namespace: m.Namespace,
		},
		Spec: appsv1.DeploymentSpec{
			Replicas: &replicas,
			Selector: &metav1.LabelSelector{
				MatchLabels: ls,
			},
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{
					Labels: ls,
				},
				Spec: corev1.PodSpec{
					Containers: []corev1.Container{{
						Image: "postgres:14.2-alpine",
						Name:  "postgres",
						Args:  []string{},
						Env: []corev1.EnvVar{
							{
								Name:  "PGDATA",
								Value: "/var/lib/postgresql/data",
							},
							{
								Name: "POSTGRES_PASSWORD",
								ValueFrom: &corev1.EnvVarSource{
									SecretKeyRef: &corev1.SecretKeySelector{
										LocalObjectReference: corev1.LocalObjectReference{
											Name: m.Name,
										},
										Key: "password",
									},
								},
							},
						},
						Ports: []corev1.ContainerPort{{
							ContainerPort: 5432,
							Name:          "postgres",
						}},
						VolumeMounts: []corev1.VolumeMount{
							{
								Name:      "data",
								MountPath: "/var/lib/postgresql/data",
								ReadOnly:  false,
							},
						},
					}},
					Volumes: []corev1.Volume{
						{
							Name:         "data",
							VolumeSource: corev1.VolumeSource{PersistentVolumeClaim: &corev1.PersistentVolumeClaimVolumeSource{ClaimName: m.Name}},
						},
					},
				},
			},
		},
	}
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

func (r *PostgresReconciler) postgresService(m *starlanev1alpha1.Postgres) *corev1.Service {

	name := m.Name
	service_type := corev1.ServiceType("")

	if m.Spec.ServiceType == "" {
		service_type = corev1.ServiceType("ClusterIP")
	} else {
		service_type = m.Spec.ServiceType
	}

	dep := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Type: service_type,
			Ports: []corev1.ServicePort{
				{Name: "postgres",
					Port:       5432,
					TargetPort: intstr.FromInt(5432),
					Protocol:   corev1.ProtocolTCP,
				},
			},
			Selector: map[string]string{"name": name},
		},
	}
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

func (r *PostgresReconciler) generateSecret(m *starlanev1alpha1.Postgres) (*corev1.Secret, error) {
	password, err := password.Generate(16, 4, 4, false, false)

	if err != nil {
		return &corev1.Secret{}, err
	} else {
		data := map[string]string{"password": password}
		dep := &corev1.Secret{
			ObjectMeta: metav1.ObjectMeta{
				Name:      m.Name,
				Namespace: m.Namespace,
			},
			StringData: data,
		}
		ctrl.SetControllerReference(m, dep, r.Scheme)
		return dep, nil
	}
}
